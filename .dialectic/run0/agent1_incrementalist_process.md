# The Case for Stage-Decomposed Verification in eaglemode-rs

**Position:** The best approach to achieving zero-tolerance golden test parity in the eaglemode-rs C++->Rust port is to decompose the rendering pipeline into independently verifiable stages and prove each stage matches C++ before composing them -- rather than continuing the current end-to-end golden test -> hypothesize -> investigate -> fix cycle.

---

## I. The Fog Problem Is Structural, Not Accidental

The eaglemode-rs rendering pipeline transforms caller intent into final pixels through a sequence of stages: (1) 9-slice subdivision produces section rectangles, (2) per-section transform setup converts logical coordinates to fixed-point image-space parameters, (3) interpolation (area sampling or adaptive) produces intermediate pixel values, (4) compositing blends those values into the target buffer. A golden test compares the output of stage 4 against a reference. When a test fails, the pixel diff tells you that *something* diverged, but it cannot tell you *where in the pipeline* the divergence was introduced.

This is not a limitation of the test infrastructure that could be resolved by better tooling or smarter agents. It is a mathematical property of composed functions. If `f = h . g` and `f(x) != f_ref(x)`, the failure is consistent with `g` diverging, `h` diverging, or both diverging in ways that partially cancel. The pixel diff -- the only signal available from an end-to-end test -- is the composition of all upstream divergences through all downstream transformations. It is, in the information-theoretic sense, a *lossy projection* of a multi-dimensional divergence space onto a single-dimensional observation.

The eaglemode-rs investigation history provides empirical proof that this is not a theoretical concern:

**Hypothesis 1 (area sampling inner loop):** The investigation examined whether the core area sampling arithmetic diverged. The area sampling inner loop was found to be a literal port. The hypothesis was disproven, though a tangential fix to `tdx_init` computation improved 5 unrelated tests. The golden test signal -- "border image pixels are wrong" -- was consistent with an area sampling divergence but the actual area sampling was correct. The fog hid the real cause.

**Hypothesis 2 (9-slice transform parameters TX/TY/TDX/TDY):** The investigation examined whether Rust computed different fixed-point transform parameters than C++ for the same input rectangle. The parameters were found to match exactly. Again, the golden test signal was consistent with a transform divergence, but the transforms were correct. The fog persisted because the test signal could not distinguish between "wrong transform input to correct area sampling" and "correct transform input to slightly different section boundary computation."

**Hypothesis 3 (PaintImageColored color mapping):** The investigation examined whether the color mapping pipeline for `PaintImageColored` diverged. The pipeline was found to be correct -- and moreover, the divergent pixels were not even produced by `PaintImageColored`. The pixel coordinates of the divergence (x=24-35, y=288-305 for checkbox) correspond to the border indicator area, not the label text area where `PaintImageColored` operates (x=170-776). The fog was so dense that the investigation was examining the wrong rendering function entirely, despite careful pixel-level analysis.

Three rounds of investigation, each consuming substantial agentic compute, each producing a correct elimination, and zero direct fixes for the target group of 21 tests. This is not bad luck or poor hypothesis selection. It is the predictable consequence of debugging a multi-stage pipeline using only the final output as a signal. The feedback loop is too long. The observation space is too narrow. The fog is structural.

The pattern matches what Vouk (1990) identified in interlanguage conversion projects: when conversion is performed in bulk and testing is done only at the system boundary, defects cluster in the interactions between converted modules, and localization requires expensive bisection. When conversion and testing proceed incrementally -- at the module boundary, then at the integration boundary -- defects are caught at their point of introduction, and localization is free.

---

## II. Intermediate Verification Would Have Caught the Root Cause Immediately

The failure catalog (2026-04-02) identifies the root cause of Groups A and B (21 tests) as "9-slice border image section boundary rounding diverges from C++." The divergent pixels cluster at section edges -- exactly the pixels that are determined by how the 9 sections are divided and how each section's transform is initialized.

Consider what would have happened if the project had a stage-level comparison harness:

**Stage 1 harness (section rectangle computation):** For a given PaintBorderImage call with inputs `(x, y, w, h, l, t, r, b, image, srcL, srcT, srcR, srcB, alpha, canvasColor, whichSubRects)`, emit the 9 section rectangles computed by the Rust `PaintBorderImage` function and the 9 section rectangles computed by the C++ `PaintBorderImage` function. Compare them.

If the section rectangles match, the divergence is downstream. If they differ, the root cause is found, and the fix is to adjust the section computation. No hypothesizing, no pixel-level forensics, no false trails.

**Stage 2 harness (transform initialization):** For each section rectangle, emit the fixed-point transform parameters (TX, TY, TDX, TDY, stride_x, stride_y, off_x, off_y) computed by Rust's `paint_9slice_section` and by C++'s `ScanlineTool::Init` (via `PaintImage -> PaintRect -> Init`). Compare them.

This is where the architectural divergence becomes visible. The C++ routes each section through `PaintImage`, which is an inline function that calls `PaintRect` with an `emImageTexture`. `PaintRect` then calls `ScanlineTool::Init`, which computes transforms using the full shared Init logic including source-rect clamping, downscale stride selection, pixel-map pointer adjustment, and area-sampling inverse-scale computation. The Rust `paint_9slice_section` reimplements this logic inline, and was previously assessed as "structurally equivalent with minor differences that don't cause divergence in normal cases." But the golden test cases exercise sub-pixel boundary conditions where "minor differences" become divergences.

A stage 2 harness would emit the exact numerical values of every transform parameter, for every section, for every failing test case. The divergence would be visible as a numerical diff, localized to the exact parameter and the exact section. The developer would see: "For the upper-left corner section at (x=0.23, y=0.23, w=2.77, h=2.77), C++ computes TDX=0x1234567 and Rust computes TDX=0x1234568. The difference is 1 LSB in 24-bit fixed-point, caused by the Rust version computing `sw as u32` before the division while C++ uses `ImgW` after source-rect clamping." The fix is obvious, the verification is immediate, and no hypothesis cascade was needed.

**Stage 3 harness (interpolated scanline values):** For each section and each scanline row, emit the interpolated pixel values before compositing. Compare them between Rust and C++.

This would catch divergences in the area sampling kernel, the adaptive interpolation table, the bilinear/bicubic filter coefficients -- every interpolation-level issue. It would also verify that a section-level fix at stage 1 or 2 propagates correctly through interpolation, without requiring a full end-to-end golden test rerun.

The key property of stage-level verification is that **diagnosis and localization are the same operation**. When a stage-level comparison fails, the divergence is in that stage by definition. There is no classification problem, no spatial analysis of pixel coordinates, no "is this from PaintImageColored or PaintBorderImage?" ambiguity. The harness answers the question by construction.

---

## III. The Infrastructure Cost Pays for Itself

Building stage-level comparison harnesses requires real engineering work:

1. **C++ instrumentation:** Modify the C++ generator (which already exists at `tests/golden/gen/`) to emit intermediate values at each pipeline stage. This means adding printf-style logging to `ScanlineTool::Init` (transform parameters), to `PaintBorderImage` (section rectangles), and to the interpolation functions (scanline pixel values). The C++ generator already builds via `make -C tests/golden/gen`, so the build infrastructure exists.

2. **Rust instrumentation:** Add equivalent logging to the Rust pipeline, either via a compile-time feature flag (`#[cfg(feature = "stage-trace")]`) or via a runtime tracing mode. The Rust `paint_9slice_section` function is 165 lines (lines 2838-3003 of `emPainter.rs`) -- adding trace points at section entry, transform computation, and interpolation output is straightforward.

3. **Comparison tooling:** A script that runs both generators on the same input, parses the stage-level traces, and reports divergences by stage. This is a differential testing harness in the sense of McKeeman (1998), applied at each pipeline stage rather than only at the output.

4. **Golden format extension:** The existing golden test format (`.painter.golden`, `.layout.golden`, etc.) may need a stage-trace variant that stores intermediate values alongside pixel data.

Estimated effort: 2-4 hours for the C++ instrumentation, 1-2 hours for the Rust instrumentation, 1-2 hours for the comparison script. Total: 4-8 hours of focused engineering.

Now consider the cost of the hypothesis cascade approach. The investigation history shows:

- **Hypothesis 1 investigation (area sampling):** Required reading and comparing the area sampling inner loop across both codebases, tracing fixed-point arithmetic manually, building mental models of carry-over state, and ultimately discovering that the carry-over was not the issue. The investigation produced a correct side-effect fix but no progress on the target group. Estimated agentic compute: 2-4 hours equivalent.

- **Hypothesis 2 investigation (transform parameters):** Required reading `ScanlineTool::Init` in C++, comparing it to `paint_9slice_section` in Rust, manually computing TDX/TDY for specific test cases, and verifying that the numerical values matched. Estimated agentic compute: 1-2 hours equivalent.

- **Hypothesis 3 investigation (PaintImageColored):** Required tracing the PaintImageColored pipeline, comparing C++ PaintScanlineIntG1/G2/G1G2 against Rust blend_colored_scanline, porting the pipeline to match C++ single-step structure, and then discovering that the divergent pixels were not from PaintImageColored at all. Estimated agentic compute: 2-4 hours equivalent.

Total spent on three disproven hypotheses: 5-10 hours equivalent. And the root cause is still unfixed.

The infrastructure cost (4-8 hours) is already at break-even after just the Group A+B investigation. But the infrastructure serves all 12 groups, not just one. The hypothesis cascade approach requires a new investigation for each group, each carrying its own fog. The stage-level approach provides a permanent diagnostic capability.

Furthermore, the hypothesis cascade approach has a hidden cost: **false confidence from correct eliminations**. After three hypotheses were disproven for Group A+B, the investigation concluded that the area sampling loop, the transform parameters, and the PaintImageColored pipeline are all correct. These are valuable findings. But they were obtained at the cost of not investigating the actual root cause -- the section rectangle computation itself and the Init logic divergence. Each correct elimination narrowed the search space but also consumed the investigation budget. A stage-level harness would have pointed to the root cause on the first run, leaving the budget available for the fix rather than the diagnosis.

---

## IV. Per-Stage Verification Eliminates the Classification Problem

The golden failure catalog (2026-04-02) classifies 37 failures into 12 groups. Each group has a "likely cause" and a "status." The classification itself required substantial investigative effort: pixel-level spatial analysis, call-stack tracing, hypothesis formation and testing. And the classification has been revised multiple times -- Group A+B was originally "missing HowTo text," then "PaintTextBoxed text rendering divergence," then finally "9-slice border image section boundary rounding." Each reclassification was correct at the time given the available evidence, and each was superseded by deeper investigation.

The classification problem arises because end-to-end golden tests conflate multiple divergence sources. A single test can have divergent pixels from multiple pipeline stages -- border rendering, text rendering, overlay rendering -- and the investigator must disentangle them using spatial analysis and call-stack reasoning. This is inherently error-prone. The first classification of Group A+B was wrong because the spatial analysis identified the divergent region but attributed it to the wrong rendering function. The second classification was wrong because it attributed the divergence to PaintTextBoxed when the actual rendering at those coordinates was PaintBorderImage.

Per-stage verification eliminates this problem entirely. If the section rectangle computation matches C++ exactly, and the transform initialization matches C++ exactly, and the interpolation output matches C++ exactly, then any remaining pixel divergence is definitionally in the compositing stage. There is no classification to perform, no spatial analysis to misinterpret, no rendering function to misattribute. Each stage is either correct or not, and the verification is a numerical comparison, not a forensic investigation.

This also eliminates the "minor differences that don't cause divergence in normal cases" trap. The previous investigation of `paint_9slice_section` found it "structurally equivalent" to C++ ScanlineTool::Init, with the assessment that differences were non-causal. But this assessment was made without per-case numerical comparison. A stage-level harness would have shown the exact cases where the differences become divergences, without requiring the investigator to reason about whether a structural difference is "minor" or not.

The Feathers (2005) seam model provides the theoretical framework. A seam is a place where you can alter behavior without editing code at that point. In the eaglemode-rs pipeline, the seams are at the stage boundaries: between section computation and transform initialization, between transform initialization and interpolation, between interpolation and compositing. Inserting verification at seams (sensing, in Feathers' terminology) converts each seam into a test point. Characterization tests at each seam capture the current C++ behavior as the specification. The Rust code at each stage is then verified against that characterization, not against the end-to-end pixel output.

---

## V. This Approach Handles the Remaining 12 Groups, Not Just Group A+B

The pipeline stages are shared across failure groups. The stage-level infrastructure serves multiple fix campaigns simultaneously:

**Section rectangle computation (Stage 1):** Groups A (15 tests), B (5 tests), and D (1 test) all involve 9-slice border image rendering. A Stage 1 harness verifies section rectangles for all three groups in a single comparison run.

**Transform initialization (Stage 2):** Groups A, B, D, and C (2 tests, star rendering) all pass through transform initialization. The C++ ScanlineTool::Init is a single shared function. The Rust code path diverges -- `paint_9slice_section` for border images, `paint_image_full` for regular images, `PaintImageColored` for colored images. A Stage 2 harness verifies that all Rust code paths produce the same transform parameters as the single C++ Init for the same inputs.

**Interpolation (Stage 3):** Groups A, B, D (area sampling / adaptive), G3 (2 tests, adaptive table rounding), and C (sub-pixel interpolation) all involve the interpolation stage. A Stage 3 harness compares interpolated scanline values between Rust and C++ for all these groups.

**Polygon rasterization (lateral stage):** Group G2 (6 tests) involves polygon rasterization, which is a separate pipeline branch. A polygon rasterization harness -- comparing edge-crossing x-coordinates per scanline between Rust and C++ -- would localize the accumulation divergence to the exact scanline and edge where `x_cur += dx_per_row` diverges from C++'s in-place `x1 += dx` mutation.

**Compositing (Stage 4):** Groups G5 (1 test, blend formula), G6 (1 test, polygon AA boundary), and portions of G7 (gradient) involve the compositing stage. A Stage 4 harness comparing pre-composited and post-composited values would isolate these to the exact blend operation.

**Gradient pipeline (lateral stage):** Group G7 (1 test, linear gradient fixed-point walk) involves a separate gradient interpolation path. The zero-tolerance failure report (2026-04-01) already identified an architectural mismatch in this pipeline: the C++ `SharedPixelFormat` hash table mediates all pixel writes, while the Rust pipeline separates hash formula computation from blend. A gradient stage harness -- comparing the 1-byte `g` values before the hash lookup -- would determine whether the divergence is in the gradient walk or in the hash mediation.

**Structural divergences (Groups G8, G9):** These involve higher-level structural differences (wrong canvas_color for PaintRect strips, wrong stroke polygon construction) that are not pure arithmetic divergences. Stage-level verification would still catch them -- a Stage 1 equivalent for polygon construction would show different vertex lists between Rust and C++.

The total coverage: 36 of 37 failing tests pass through one or more shared pipeline stages. Building the stage-level infrastructure once provides diagnostic capability for all of them.

---

## VI. Per-Divergence Design Decisions Prevent Wasted Work

When a stage-level divergence is found, the correct response depends on the nature of the divergence:

1. **Literal port error:** The Rust code implements a different formula than C++ due to a porting mistake. Fix: change the Rust formula to match C++. Example: the `tdx_init` fix that resolved 5 tests.

2. **Architectural divergence:** The Rust code reimplements shared C++ logic inline, and the reimplementation differs in edge cases. Fix: route through the shared logic (match C++ architecture, not just formulas). Example: `paint_9slice_section` reimplements `ScanlineTool::Init` inline instead of routing through the shared `PaintImage -> PaintRect -> Init` path.

3. **Intentional idiom difference:** The Rust code uses a different approach that is allowed by the CLAUDE.md rules for non-golden-tested paths, but happens to feed a golden test. Fix: port the C++ approach for the golden-tested path, keep the Rust idiom for other paths.

4. **C++ bug reproduction:** The C++ code has a quirk (e.g., `(x*257+0x8073)>>16` rounding bias) that the Rust code must reproduce for parity. Fix: port the exact C++ formula, document the bias.

5. **Test setup error:** The golden test reference data was generated with different parameters than the Rust test. Fix: align the test setup. (The failure report identifies this as a real risk: "Phase 5 golden tests may have wrong setup.")

Without stage-level verification, the investigator must hypothesize which category applies before knowing the specific divergence. This leads to wasted work: the PaintImageColored investigation ported the pipeline to match C++ single-step structure (fix category 2), but the divergence was not from PaintImageColored (wrong diagnosis). The fix was correct engineering but applied to the wrong function.

With stage-level verification, the investigator sees the specific numerical divergence first and then selects the appropriate fix category. "Section rectangle X differs by 0.003 pixels between Rust and C++ because Rust computes `iw - sl - sr` in f64 while C++ computes `srcW - srcL - srcR` in int" is a specific, actionable finding that maps directly to fix category 1 (literal port error in source-center-region computation).

This also prevents the anti-pattern identified in the failure report: "Continuing to look for formula fixes after evidence showed architectural mismatch." When the stage-level harness shows that transforms match but interpolated values differ, the investigator knows the divergence is in interpolation, not in transform initialization. They will not waste time on formula fixes in the transform code.

---

## VII. Golden Tests Remain as the Composition Verification Layer

This proposal does not replace golden tests. It adds intermediate verification so that golden tests serve their proper function: verifying that correctly-implemented stages compose correctly.

The golden test suite (241 tests, 204 passing at zero tolerance) remains the ultimate arbiter of parity. The stage-level harnesses are diagnostic tools, not replacement tests. They serve the same role that unit tests serve relative to integration tests: they localize failures, they speed debugging, and they provide confidence that individual components are correct before composition.

The key insight is that the golden tests are currently serving double duty as both composition verification and component verification. They are the only signal available for both "does the pipeline produce correct output?" and "does each pipeline stage produce correct intermediate values?" This conflation is the root cause of the fog. Separating the two functions -- stage-level harnesses for component verification, golden tests for composition verification -- eliminates the fog and restores each testing layer to its proper scope.

When a golden test fails after all stage-level harnesses pass, the investigator knows the divergence is in the composition itself: the way stages are connected, the order of operations, the data flow between stages. This is a much smaller search space than "somewhere in the entire pipeline." And composition-level divergences are typically architectural (matching the feedback from the failure report: "The actual problem is architectural") rather than arithmetic, making them easier to diagnose from code review.

---

## VIII. The Epistemology of Porting: Why End-to-End Tests Are Insufficient for Interlanguage Conversion

The eaglemode-rs project is an interlanguage conversion with a specific fidelity requirement: zero-tolerance golden test parity on pixel output. This means the Rust implementation must produce *identical* pixels to the C++ implementation for every golden test case. Not approximately identical. Not within-tolerance. Identical.

This requirement makes the end-to-end approach fundamentally inadequate. Consider: the rendering pipeline has approximately 4 stages, each with approximately 10-50 numerical parameters. A single golden test exercises one path through all 4 stages. A pixel divergence of 1 LSB in one channel could result from:
- A 1-LSB error in one parameter at one stage
- Compensating errors at two stages that partially cancel
- A correct-but-differently-ordered sequence of operations that produces different rounding
- An architectural difference that changes which hash table entry is used for compositing

The end-to-end test cannot distinguish between these cases. The investigator must. And the investigator's hypothesis selection is biased by which stage they understand best, which code they read most recently, and which divergence pattern they have seen before. The three disproven hypotheses for Group A+B demonstrate this bias: the investigations focused on area sampling (complex, recently fixed), transform parameters (easy to verify numerically), and PaintImageColored (recently ported) -- not on the section rectangle computation (structurally simple, assumed correct) or the Init logic reimplementation (previously assessed as "equivalent").

Stage-level verification removes the investigator's biases from the diagnostic process. The harness checks every stage, for every test case, with numerical precision. It does not skip stages that "look correct" or prioritize stages that "seem likely." It treats the pipeline as what it is: a sequence of functions, each of which must be verified independently before the composition can be trusted.

This is the essence of Vouk's (1990) back-to-back testing strategy for interlanguage conversion: the original version is always available as an oracle. Use it at every conversion step, not just at the system boundary. The cost of building the harnesses is paid once. The cost of not building them is paid on every investigation, compounding with each disproven hypothesis, until the fog becomes impenetrable and the investigators resort to code review and reasoning rather than empirical verification.

The eaglemode-rs project is at that inflection point now. Three hypothesis cascades have produced zero direct fixes for the largest failure group. The remaining uninvestigated areas -- section rectangle computation, Init logic divergence -- are precisely the areas that a stage-level harness would have identified immediately. The choice is between continuing to hypothesize in the fog, or building the infrastructure to see clearly.

The infrastructure is the correct investment. Build the harnesses. Verify each stage. Fix what the harnesses reveal. Let the golden tests confirm the composition. This is the methodology that Vouk prescribed, that Feathers operationalized, and that McKeeman automated. It is the methodology that the project's own failure history validates by counterexample. It is the path to zero tolerance.
